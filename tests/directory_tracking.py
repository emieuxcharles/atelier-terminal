"""Verify bundled zsh notifications in a PTY, without user plugins or external z."""
import errno
import os
from pathlib import Path
import pty
import re
import shlex
import subprocess
import tempfile

hook = Path(__file__).resolve().parents[1] / 'resources/directory-tracking.zsh'
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp).resolve()
    child = root / 'espaces é | saut\nligne'
    child.mkdir()
    (root / "subshell-only").mkdir()
    (root / 'alias').symlink_to(child)
    master, slave = pty.openpty()
    env = dict(os.environ, ATELIER_SSH_TOKEN='test', ATELIER_CWD_OWNER='')
    script = '\n'.join([
        'function user_chpwd() { print USER_HOOK; }',
        'chpwd_functions+=(user_chpwd)',
        f'source {shlex.quote(str(hook))}',
        f'source {shlex.quote(str(hook))}',
        f'cd {shlex.quote(str(child))}',
        '_atelier_report_cwd', '_atelier_report_cwd',
        f'cd {shlex.quote(str(root / "alias"))}',
        f'cd {shlex.quote(str(root))}',
        f'(cd {shlex.quote(str(root / "subshell-only"))})',
        f'cd {shlex.quote(str(child))} > {shlex.quote(str(root / "redirected.txt"))}',
        '_atelier_report_cwd',
        f'cd {shlex.quote(str(root))}',
        'cd /atelier-directory-that-does-not-exist',
        'exit', '',
    ])
    shell_args = ['/bin/zsh', '-d', '-f', '-i', '-c', script]
    if os.environ.get('ATELIER_TEST_LOGIN'):
        script = '\n'.join(line for line in script.split('\n') if not line.startswith('source '))
        shell_args = ['/bin/zsh', '-l', '-i', '-c', script]
    proc = subprocess.Popen(shell_args, cwd=root, env=env,
                            stdin=subprocess.DEVNULL, stdout=slave, stderr=slave)
    os.close(slave)
    chunks = []
    while True:
        try:
            data = os.read(master, 4096)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            break
        if not data:
            break
        chunks.append(data)
    os.close(master)
    proc.wait(timeout=5)
    output = b''.join(chunks)
    encoded = re.findall(rb'\x1b\]2;ATELIER_CWD:test:([0-9a-f]+)\x07', output)
    paths = [os.fsdecode(bytes.fromhex(value.decode())) for value in encoded]
    assert paths == [str(root), str(child), str(root), str(child), str(root)], (paths, output)
    assert output.count(b'USER_HOOK\r\n') == 5, output
    assert b'ATELIER_CWD' not in (root / 'redirected.txt').read_bytes()
print('Directory tracking: startup, cd, spaces/Unicode/control bytes, symlink dedup, existing hooks, no repeated prompt events passed')
