import asyncio
import copy
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import tempfile
import sys
import os
import unittest
from unittest.mock import patch

path=Path(__file__).resolve().parents[1]/'streamdeck-pedal-actions'
loader=importlib.machinery.SourceFileLoader('pedal',str(path)); spec=importlib.util.spec_from_loader(loader.name,loader); pedal=importlib.util.module_from_spec(spec);loader.exec_module(pedal)
real_run=pedal.run

class DaemonTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.tmp=tempfile.TemporaryDirectory();p=Path(self.tmp.name)
        self.d=pedal.Daemon(p/'config.json',p);self.d.connected=True;self.d.startup=True
        self.calls=[];self.owner=object();self.muted=True
        async def fake(*args,**kwargs):
            self.calls.append(args)
            if args[:2]==('pactl','get-source-mute'):return 'Mute: yes' if self.muted else 'Mute: no'
            if args[:2]==('pactl','set-source-mute'): self.muted=args[3]=='1';return ''
            if args[:3]==('pactl','--format=json','list'):return json.dumps([{'name':'mic','description':'Test microphone'}])
            if args[:2]==('busctl','--user') and args[-1]=='list':return 'org.mpris.MediaPlayer2.test 123 process\n'
            if 'get-property' in args:return 's "Playing"'
            return ''
        self.patch=patch.object(pedal,'run',fake);self.patch.start()
        self.which=patch.object(pedal.shutil,'which',lambda x:x);self.which.start()
    async def asyncTearDown(self):
        await self.d.release_all();await self.drain();self.patch.stop();self.which.stop();self.tmp.cleanup()
    async def drain(self):
        while self.d.tasks:await asyncio.gather(*list(self.d.tasks))
    async def press(self,i):
        a=[False]*3;self.d.input(a);a[i]=True;self.d.input(a);await self.drain()
    async def test_original_mapping_press_edges(self):
        self.d.input([False]*3)
        self.d.input([True,True,True]);await self.drain()
        self.d.input([True,True,True]);await self.drain()
        self.assertEqual(len(self.calls),3)
        self.assertIn(('voxtype','record','toggle'),self.calls)
        self.assertIn(('hyprctl','eval','hl.dispatch(hl.dsp.focus({ workspace = "e-1" }))'),self.calls)
        self.assertIn(('hyprctl','eval','hl.dispatch(hl.dsp.focus({ workspace = "e+1" }))'),self.calls)
    async def test_test_mode_suppresses_and_requires_release(self):
        await self.d.rpc({'op':'test_start'},self.owner)
        await self.press(1);self.assertEqual(self.calls,[])
        await self.d.rpc({'op':'test_end'},self.owner)
        self.d.input([False,True,False]);await self.drain();self.assertEqual(self.calls,[])
        await self.press(1);self.assertIn(('voxtype','record','toggle'),self.calls)
    async def test_paused_state_survives_test(self):
        await self.d.rpc({'op':'enabled','value':False},self.owner)
        await self.d.rpc({'op':'test_start'},self.owner);await self.d.end_test();await self.press(1)
        self.assertFalse(self.d.config['enabled']);self.assertEqual(self.calls,[])
    async def test_only_owner_can_end_test(self):
        await self.d.rpc({'op':'test_start'},self.owner)
        await self.d.rpc({'op':'test_end'},object());self.assertIs(self.d.test_owner,self.owner)
        with self.assertRaises(ValueError):await self.d.rpc({'op':'test_start'},object())
    async def test_persist_custom_preserves_other_pedals(self):
        p=copy.deepcopy(pedal.BUILTINS[0]);p.update(id='',name='My keys');p['actions'][1]={'type':'shortcut','key':'m','modifiers':['ctrl','shift']}
        await self.d.rpc({'op':'save','preset':p},self.owner)
        fresh=pedal.Daemon(self.d.config_path,self.d.runtime)
        self.assertEqual(fresh.active()['actions'][0],pedal.BUILTINS[0]['actions'][0]);self.assertEqual(fresh.active()['actions'][2],pedal.BUILTINS[0]['actions'][2])
        self.assertEqual(fresh.active()['name'],'My keys')
        await self.press(1);self.assertIn(('wtype','-M','ctrl','-M','shift','-k','m','-m','shift','-m','ctrl'),self.calls)
    async def test_validation_does_not_overwrite_config(self):
        await self.d.rpc({'op':'enabled','value':True},self.owner);before=self.d.config_path.read_bytes()
        p={'id':'','name':'Bad','actions':[{'type':'none'},{'type':'shortcut','key':'$(bad)'},{'type':'none'}]}
        with self.assertRaises(ValueError):await self.d.rpc({'op':'save','preset':p},self.owner)
        self.assertEqual(self.d.config_path.read_bytes(),before)
    async def test_builtin_immutable(self):
        with self.assertRaises(ValueError):await self.d.rpc({'op':'save','preset':copy.deepcopy(pedal.BUILTINS[0])},self.owner)
        with self.assertRaises(ValueError):await self.d.rpc({'op':'activate','preset':'push-to-talk'},self.owner)
    async def mic_preset(self):
        await self.d.rpc({'op':'save','preset':{'id':'','name':'Hold','actions':[{'type':'none'},{'type':'mic_hold','source':'mic'},{'type':'none'}]}},self.owner)
        self.calls=[]
    async def test_microphone_restores_on_release(self):
        await self.mic_preset();await self.press(1);self.assertFalse(self.muted)
        self.d.input([False]*3);await self.drain();self.assertTrue(self.muted);self.assertFalse(self.d.holds)
    async def test_microphone_preserves_unmuted_initial_state(self):
        self.muted=False;await self.mic_preset();await self.press(1);self.d.input([False]*3);await self.drain();self.assertFalse(self.muted)
    async def test_disconnect_restores_microphone(self):
        await self.mic_preset();await self.press(1);self.d.disconnect();await self.drain();self.assertTrue(self.muted);self.assertFalse(self.d.connected)
    async def test_pause_restores_microphone(self):
        await self.mic_preset();await self.press(1);await self.d.rpc({'op':'enabled','value':False},self.owner);self.assertTrue(self.muted)
    async def test_switch_restores_microphone(self):
        await self.mic_preset();await self.press(1);await self.d.rpc({'op':'activate','preset':'workspaces'},self.owner);self.assertTrue(self.muted)
    async def test_release_before_hold_task_does_not_unmute(self):
        await self.mic_preset();self.d.input([False]*3);self.d.input([False,True,False]);self.d.input([False]*3);await self.drain()
        self.assertTrue(self.muted);self.assertNotIn(('pactl','set-source-mute','mic','0'),self.calls)
    async def test_recover_microphone_after_crash(self):
        await self.mic_preset();await self.press(1)
        fresh=pedal.Daemon(self.d.config_path,self.d.runtime);await fresh.recover();self.assertTrue(self.muted)
    async def test_command_uses_argv_not_shell(self):
        p={'id':'','name':'Command','actions':[{'type':'command','executable':'echo','arguments':'"two words" "$(touch /tmp/no)"','cwd':''},{'type':'none'},{'type':'none'}]}
        await self.d.rpc({'op':'save','preset':p},self.owner);await self.press(0)
        self.assertIn(('echo','two words','$(touch /tmp/no)'),self.calls)
    async def test_error_visible_per_pedal(self):
        async def bad(*a,**k):raise RuntimeError('Voxtype unavailable')
        with patch.object(pedal,'run',bad):await self.press(1)
        self.assertEqual(self.d.errors[1],'Voxtype unavailable');self.assertEqual(self.d.errors[0],'')
    async def test_first_held_report_does_not_fire(self):
        self.d.input([False,True,False]);await self.drain();self.assertEqual(self.calls,[])
        await self.press(1);self.assertEqual(len(self.calls),1)
    async def test_malformed_config_is_preserved(self):
        self.d.config_path.write_text('{broken')
        fresh=pedal.Daemon(self.d.config_path,self.d.runtime)
        self.assertFalse(fresh.config['enabled']);self.assertTrue(fresh.config_error)
        with self.assertRaises(ValueError):await fresh.save(pedal.defaults())
        self.assertEqual(self.d.config_path.read_text(),'{broken')
    async def test_media_action_routes_mpris(self):
        await self.d.rpc({'op':'activate','preset':'media'},self.owner);await self.press(1)
        self.assertIn(('busctl','--user','call','org.mpris.MediaPlayer2.test','/org/mpris/MediaPlayer2','org.mpris.MediaPlayer2.Player','PlayPause'),self.calls)
    async def test_rpc_connection_loss_exits_test(self):
        sock=Path(self.tmp.name)/'test.sock'
        server=await asyncio.start_unix_server(self.d.client,path=sock)
        r,w=await asyncio.open_unix_connection(sock);await r.readline()
        w.write(b'{"op":"test_start","id":2}\n');await w.drain()
        while True:
            m=json.loads(await r.readline())
            if m.get('type')=='result':break
        self.assertTrue(self.d.test_owner is not None)
        w.close();await w.wait_closed();await asyncio.sleep(.05)
        self.assertIsNone(self.d.test_owner);server.close();await server.wait_closed()

    async def test_shared_microphone_restores_after_last_owner(self):
        p={'id':'','name':'Two holds','actions':[{'type':'mic_hold','source':'mic'},{'type':'mic_hold','source':'mic'},{'type':'none'}]}
        await self.d.rpc({'op':'save','preset':p},self.owner)
        self.d.input([False]*3);self.d.input([True,True,False]);await self.drain();self.assertFalse(self.muted)
        self.d.input([False,True,False]);await self.drain();self.assertFalse(self.muted)
        self.d.input([False]*3);await self.drain();self.assertTrue(self.muted)
    async def test_failed_restore_retains_recovery_and_can_retry(self):
        await self.mic_preset();await self.press(1)
        fake=pedal.run
        async def fail(*args,**kw):
            if args==('pactl','set-source-mute','mic','1'):raise RuntimeError('Disconnected')
            return await fake(*args,**kw)
        with patch.object(pedal,'run',fail):await self.d.rpc({'op':'enabled','value':False},self.owner)
        self.assertEqual(self.d.holds['mic']['owners'],set())
        self.assertTrue(json.loads((self.d.runtime/'microphone-recovery.json').read_text())['mic'])
        await self.d.release(-1);self.assertTrue(self.muted);self.assertFalse(self.d.holds)

    async def test_status_caches_dependencies_until_refresh(self):
        with patch.object(pedal.shutil,'which',return_value=None) as which:
            for _ in range(10): self.d.status();self.d.publish()
            which.assert_not_called()
            await self.d.rpc({'op':'refresh'},self.owner)
            self.assertEqual(which.call_count,len({v[1] for v in pedal.ACTIONS.values() if v[1]}))
            self.assertFalse(next(a for a in self.d.status()['actions'] if a['value']=='dictation')['available'])

    async def test_socket_deduplicates_state_but_initial_state_is_always_sent(self):
        socket=self.d.runtime/'test.sock'
        server=await asyncio.start_unix_server(self.d.client,path=socket)
        clients=[]
        try:
            self.d.publish()
            for _ in range(2):
                r,w=await asyncio.open_unix_connection(socket);clients.append((r,w))
                self.assertEqual(json.loads(await r.readline())['type'],'state')
            for _ in range(10): self.d.publish()
            for r,w in clients:
                with self.assertRaises(asyncio.TimeoutError):await asyncio.wait_for(r.readline(),.03)
            self.d.errors[0]='Changed';self.d.publish()
            for r,w in clients:
                self.assertEqual(json.loads(await asyncio.wait_for(r.readline(),.2))['state']['errors'][0],'Changed')
        finally:
            for r,w in clients:w.close();await w.wait_closed()
            server.close();await server.wait_closed()

    async def test_connected_watch_sleeps_until_state_changes_and_expires_lease(self):
        self.d.fd=123  # Already connected: watch must never inspect or poll HID.
        real_wait=asyncio.wait_for
        with patch.object(pedal,'find_pedal') as discover, patch.object(pedal.asyncio,'wait_for',wraps=real_wait) as timed_wait:
            task=asyncio.create_task(self.d.watch())
            try:
                await asyncio.sleep(.03)
                discover.assert_not_called();timed_wait.assert_not_called()
                await self.d.rpc({'op':'test_start'},self.owner)
                self.d.test_until=pedal.time.monotonic()+.03
                await self.d.rpc({'op':'test_heartbeat'},self.owner)
                await asyncio.sleep(.06)
                self.assertIs(self.d.test_owner,self.owner)
                self.d.test_until=pedal.time.monotonic()+.02;self.d.wake.set()
                await asyncio.sleep(.06)
                self.assertIsNone(self.d.test_owner)
                self.assertTrue(timed_wait.called)
                timed_wait.reset_mock()
                await asyncio.sleep(.03);timed_wait.assert_not_called()
            finally:
                task.cancel();await asyncio.gather(task,return_exceptions=True);self.d.fd=None

    async def test_watch_retries_discovery_and_failed_restore_without_busy_loop(self):
        self.d.holds={'mic':{'muted':True,'owners':set()}}
        async def fail(*args,**kwargs):raise RuntimeError('Disconnected')
        with patch.object(pedal,'find_pedal',return_value=None) as discover, patch.object(pedal,'run',side_effect=fail) as run:
            task=asyncio.create_task(self.d.watch())
            try:
                await asyncio.sleep(.08)
                self.assertEqual(discover.call_count,1);self.assertEqual(run.call_count,1)
                for _ in range(5):self.d.publish();await asyncio.sleep(.01)
                self.assertEqual(discover.call_count,1);self.assertEqual(run.call_count,1)
                await asyncio.sleep(2)
                self.assertEqual(discover.call_count,2);self.assertEqual(run.call_count,2)
            finally:task.cancel();await asyncio.gather(task,return_exceptions=True)

class ProcessTests(unittest.IsolatedAsyncioTestCase):
    async def test_daemon_shutdown_with_connected_client(self):
        with tempfile.TemporaryDirectory() as tmp:
            program = """
import asyncio, importlib.machinery, importlib.util, sys
loader=importlib.machinery.SourceFileLoader('pedal',sys.argv[1])
spec=importlib.util.spec_from_loader(loader.name,loader)
m=importlib.util.module_from_spec(spec);loader.exec_module(m)
m.find_pedal=lambda:None
async def fake(*args,**kwargs):
    return '[]' if 'pactl' in args else 'enabled'
m.run=fake
asyncio.run(m.Daemon().serve())
"""
            env=dict(os.environ,XDG_RUNTIME_DIR=tmp,XDG_CONFIG_HOME=tmp)
            proc=await asyncio.create_subprocess_exec(sys.executable,'-c',program,str(path),env=env,stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.PIPE)
            writer=None
            try:
                socket=Path(tmp)/'foot-pedal/control.sock'
                for _ in range(100):
                    if socket.exists():break
                    await asyncio.sleep(.01)
                reader,writer=await asyncio.open_unix_connection(socket)
                await reader.readline()
                proc.terminate()
                await asyncio.wait_for(proc.wait(),2)
                self.assertEqual(proc.returncode,0,(await proc.stderr.read()).decode())
            finally:
                if writer:writer.close();await writer.wait_closed()
                if proc.returncode is None:proc.kill();await proc.wait()

    async def test_actual_command_timeout(self):
        with self.assertRaisesRegex(RuntimeError, 'timed out'):
            await real_run('/usr/bin/sleep','10',timeout=.05)
    async def test_actual_process_output_is_bounded(self):
        result=await real_run(sys.executable,'-c','print("x" * 1000000)')
        self.assertEqual(len(result),65536)
    async def test_actual_argv_never_expands_shell(self):
        result=await real_run('/usr/bin/printf','%s','$(echo should-not-run)')
        self.assertEqual(result,'$(echo should-not-run)')

if __name__=='__main__':unittest.main()
