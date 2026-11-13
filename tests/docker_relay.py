"""Real PTY tests of the Docker relay; no Docker engine or network involved."""
import errno, os, pty, subprocess, tempfile
from pathlib import Path
wrapper = Path(__file__).resolve().parents[1] / 'resources/docker-wrapper.zsh'
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    fake = root / 'docker'
    fake.write_text('''#!/bin/zsh -f
if [[ "$1" == context && "$2" == inspect ]]; then
  print -r -- "${TEST_ENDPOINT:-unix:///var/run/docker.sock}"
  exit 0
fi
printf 'ARG:<%s>\\n' "$@"
exit 42
''')
    fake.chmod(0o700)
    env = dict(os.environ, PATH=f'{root}:/usr/bin:/bin', ATELIER_SSH_TOKEN='test-token')
    cases = [
        (['exec', '-ti', 'wordpress-1', 'bash'], 'unix:///var/run/docker.sock|wordpress-1'),
        (['--context', 'remote', 'exec', '-it', '--user', 'root', 'php', 'sh'], 'ssh://user@server|php'),
        (['--host=tcp://localhost:2375', 'exec', '--interactive', '--tty', 'php', 'sh'], 'tcp://localhost:2375|php'),
        (['exec', '-it', '-e', 'X=a b', '--workdir=/app', 'php', 'printf', 'hello; untouched'], 'unix:///var/run/docker.sock|php'),
        (['ps'], None),
        (['exec', '-d', 'php', 'sh'], None),
        (['exec', 'php', 'echo', 'test'], None),
        (['exec', '--help'], None),
    ]
    for args, expected in cases:
        case_env = dict(env, TEST_ENDPOINT='ssh://user@server' if 'remote' in args else 'unix:///var/run/docker.sock')
        master, slave = pty.openpty()
        proc = subprocess.Popen(['/bin/zsh', '-f', str(wrapper), *args], env=case_env, stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        chunks=[]
        while True:
            try: data=os.read(master,4096)
            except OSError as error:
                if error.errno != errno.EIO: raise
                break
            if not data: break
            chunks.append(data)
        os.close(master)
        output=b''.join(chunks).decode()
        assert proc.wait(timeout=5)==42, output
        for arg in args: assert f'ARG:<{arg}>' in output, output
        if expected:
            assert output.startswith(f'\x1b]2;ATELIER_DOCKER:test-token:exec:{expected}\x07'), repr(output)
            assert output.endswith('\x1b]2;ATELIER_DOCKER:test-token:local\x07'), repr(output)
        else: assert 'ATELIER_DOCKER:' not in output, repr(output)
    out=subprocess.run(['/bin/zsh','-f',str(wrapper),'exec','-it','php','sh'],env=env,capture_output=True)
    assert out.returncode==42 and b'ATELIER_DOCKER:' not in out.stdout
print('Docker relay: arguments, local/SSH/TCP, return after failure, detached and piped output passed')

# Interruptions must restore the badge as well.
import os,pty,subprocess,tempfile,select,time,signal
from pathlib import Path
with tempfile.TemporaryDirectory() as tmp:
 p=Path(tmp)/'docker';p.write_text('#!/bin/zsh -f\nif [[ "$1" == context ]]; then print unix:///docker.sock; exit; fi\nprint READY\nsleep 30\n');p.chmod(0o700)
 master,slave=pty.openpty()
 proc=subprocess.Popen(['/bin/zsh','-f',str(wrapper),'exec','-it','php','sh'],stdin=slave,stdout=slave,stderr=slave,start_new_session=True,env=dict(os.environ,PATH=tmp+':/usr/bin:/bin',ATELIER_SSH_TOKEN='test'))
 os.close(slave);data=b'';deadline=time.monotonic()+5
 while b'READY' not in data and time.monotonic()<deadline:
  if select.select([master],[],[],.1)[0]: data+=os.read(master,4096)
 os.killpg(proc.pid,signal.SIGINT)
 while time.monotonic()<deadline:
  if select.select([master],[],[],.1)[0]:
   try: chunk=os.read(master,4096)
   except OSError: break
   if not chunk: break
   data+=chunk
 print('exit',proc.wait(timeout=1),'restored',b'ATELIER_DOCKER:test:local' in data)
 assert b'ATELIER_DOCKER:test:local' in data,repr(data)
 os.close(master)

# A PATH alias (macOS /var -> /private/var) must never resolve back to the relay.
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    private = root / 'private'
    private.mkdir()
    relay = private / 'docker'
    relay.write_bytes(wrapper.read_bytes())
    relay.chmod(0o700)
    alias = root / 'alias'
    alias.symlink_to(private, target_is_directory=True)
    real = root / 'real'
    real.mkdir()
    (real / 'docker').write_text('#!/bin/sh\nprintf "REAL_DOCKER:%s\\n" "$*"\n')
    (real / 'docker').chmod(0o700)
    env = dict(os.environ, PATH=f'{alias}:{private}:{alias}:{real}:/usr/bin:/bin')
    output = subprocess.run([str(alias / 'docker'), 'compose', 'ps'], env=env, capture_output=True, text=True, timeout=2)
    assert output.returncode == 0, output.stderr
    assert output.stdout == 'REAL_DOCKER:compose ps\n', output.stdout
print('Docker relay: installed PATH aliases and duplicates do not recurse')
