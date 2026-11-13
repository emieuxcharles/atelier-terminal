"""Isolated two-pane idle measurement; never loads the user's workspace/config."""
import json, os, subprocess, sys, tempfile, time
from pathlib import Path

def sample(pid):
    fields=subprocess.check_output(['ps','-p',str(pid),'-o','time=,rss='],text=True).split()
    minutes,seconds=fields[0].split(':')
    return int(minutes)*60+float(seconds),int(fields[1])
with tempfile.TemporaryDirectory(prefix='atelier-perf-') as tmp:
    root=Path(tmp)
    state={'version':1,'projects':[{'directory':tmp,'panes':2,'focused':0}], 'theme':'dark'}
    (root/'state.json').write_text(json.dumps(state))
    env=dict(os.environ,HOME=tmp,SHELL='/bin/sh',ATELIER_STATE_FILE=str(root/'state.json'))
    with open(root/'stderr','w+') as log:
        proc=subprocess.Popen([sys.argv[1]],env=env,stdout=log,stderr=log)
        try:
            time.sleep(float(os.environ.get("ATELIER_PERF_WARMUP", "3")))
            if proc.poll() is not None:
                log.seek(0);raise RuntimeError(log.read())
            start,rss1=sample(proc.pid);t=time.monotonic()
            time.sleep(6)
            end,rss2=sample(proc.pid);elapsed=time.monotonic()-t
            print(json.dumps({'binary':sys.argv[1],'interval_s':round(elapsed,2),'cpu_seconds':round(end-start,3),'cpu_percent_one_core':round(100*(end-start)/elapsed,2),'rss_mib':round(rss2/1024,1),'rss_delta_mib':round((rss2-rss1)/1024,1)}))
        finally:
            proc.terminate()
            try:proc.wait(timeout=3)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
