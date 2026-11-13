"""Exercise the SSH relay in a PTY without any network connection."""
import errno
import os
from pathlib import Path
import pty
import subprocess
import tempfile

wrapper = Path(__file__).resolve().parents[1] / 'resources/ssh-wrapper.zsh'
with tempfile.TemporaryDirectory() as tmp:
    fake = Path(tmp) / 'ssh'
    fake.write_text('#!/bin/zsh -f\nprintf "ARG:<%s>\\n" "$@"\nexit 42\n')
    fake.chmod(0o700)
    env = dict(os.environ, ATELIER_REAL_SSH=str(fake), ATELIER_SSH_TOKEN='test-token')
    for args, target in [(['--', 'my-server'], 'my-server'), (['-p', '2222', 'user@host', 'echo hello'], 'user@host'), (['-G', 'host'], None)]:
        master, slave = pty.openpty()
        proc = subprocess.Popen(['/bin/zsh', '-f', str(wrapper), *args], env=env, stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        chunks = []
        while True:
            try:
                chunk = os.read(master, 4096)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            chunks.append(chunk)
        os.close(master)
        assert proc.wait(timeout=5) == 42
        output = b''.join(chunks).decode()
        for arg in args:
            assert f'ARG:<{arg}>' in output, output
        if target:
            start = f'\x1b]2;ATELIER_SSH:test-token:ssh:{target}\x07'
            end = '\x1b]2;ATELIER_SSH:test-token:local\x07'
            assert output.startswith(start), repr(output)
            assert output.endswith(end), repr(output)
        else:
            assert 'ATELIER_SSH:' not in output
    output = subprocess.run(['/bin/zsh', '-f', str(wrapper), 'host'], env=env, capture_output=True)
    assert output.returncode == 42
    assert b'ATELIER_SSH:' not in output.stdout
print('SSH relay: PTY launch/return, failure status, arguments, -G and piped output passed')
