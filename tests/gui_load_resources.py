"""Opt-in native GUI load/idle audit, with isolated project state and synthetic shells."""
import json, os, subprocess, sys, tempfile, time
from pathlib import Path

def sample(pid):
    fields=subprocess.check_output(['ps','-p',str(pid),'-o','time=,rss='],text=True).split()
    m,s=fields[0].split(':')
    return int(m)*60+float(s),int(fields[1])

def children(pid):
    rows=[tuple(map(int,line.split())) for line in subprocess.check_output(['ps','-axo','pid=,ppid='],text=True).splitlines()]
    result={pid}
    while True:
        more={child for child,parent in rows if parent in result}
        if more <= result: return result-{pid}
        result |= more

for count in [1,4,8]:
    with tempfile.TemporaryDirectory(prefix='atelier-gui-audit-') as tmp:
        root=Path(tmp)
        shell=root/'synthetic-shell'
        shell.write_text('''#!/usr/bin/python3
import os,sys,time
from pathlib import Path
root=Path(os.environ['ATELIER_AUDIT_ROOT'])
(root/('pid-'+str(os.getpid()))).write_text(str(os.getpid()))
start=time.monotonic()
block=('audit '+ 'x'*60+'\\r\\n')*1000
for _ in range(100): sys.stdout.write(block)
sys.stdout.flush()
(root/('done-'+str(os.getpid()))).write_text(str(time.monotonic()-start))
for line in sys.stdin: print(line,flush=True)
''')
        shell.chmod(0o700)
        state=root/'state.json'
        state.write_text(json.dumps({'version':1,'projects':[{'directory':tmp,'panes':count,'focused':0}], 'theme':'dark'}))
        env=dict(os.environ,SHELL=str(shell),ATELIER_STATE_FILE=str(state),ATELIER_AUDIT_ROOT=tmp)
        with open(root/'output','w') as log:
            start=time.monotonic()
            proc=subprocess.Popen([sys.argv[1]],env=env,stdout=log,stderr=log)
            tracked=set()
            try:
                deadline=start+30
                while len(list(root.glob('done-*')))<count:
                    if proc.poll() is not None: raise RuntimeError('app exited: '+(root/'output').read_text())
                    if time.monotonic()>deadline: raise TimeoutError('log writers stalled')
                    time.sleep(.05)
                output_wall=time.monotonic()-start
                time.sleep(2)
                tracked=children(proc.pid)
                cpu1,rss1=sample(proc.pid)
                time.sleep(3)
                cpu2,rss2=sample(proc.pid)
                print(json.dumps({'binary':sys.argv[1],'panes':count,'lines_per_pane':100000,'startup_and_write_s':round(output_wall,3),'cpu_total_s':cpu1,'idle_cpu_s_over_3s':round(cpu2-cpu1,3),'rss_mib':round(rss2/1024,1),'rss_idle_delta_mib':round((rss2-rss1)/1024,1),'child_count':len(tracked)}),flush=True)
            finally:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
                # The synthetic child pids are scoped to this run only.
                tracked |= {int(p.name[4:]) for p in root.glob('pid-*')}
                deadline=time.monotonic()+3
                def alive(pid):
                    try:os.kill(pid,0);return True
                    except ProcessLookupError:return False
                while any(alive(p) for p in tracked) and time.monotonic()<deadline:time.sleep(.05)
                survivors=[p for p in tracked if alive(p)]
                print(json.dumps({'after_app_exit_survivors':survivors}),flush=True)
                for pid in survivors:
                    try:os.kill(pid,15)
                    except ProcessLookupError:pass
