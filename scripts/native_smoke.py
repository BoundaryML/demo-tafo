#!/usr/bin/env python3
"""Exercise native focus behavior with deterministic selections and real BAML cycles.
Temporarily restarts the listener and switches among already-open apps.
Tests the control API rather than physical global-key delivery. Uses isolated history and disables telemetry.
"""
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_APP = ROOT / 'dist/Tafo.app'
APP = Path(os.environ.get('SMART_SWITCH_SMOKE_APP', str(DEFAULT_APP)))
BIN = APP / 'Contents/MacOS/Tafo'
REAL_STATE = Path.home() / 'Library/Application Support/Smart Switch'


def control(state, command):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2)
        client.connect(str(state / 'control.sock'))
        client.sendall(command.encode())
        client.shutdown(socket.SHUT_WR)
        return json.loads(client.recv(4096))


def front():
    return subprocess.check_output(['/usr/bin/lsappinfo', 'front'], text=True).strip()


def wait_for(predicate, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return True
        time.sleep(.025)
    return False


def state_hash():
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in REAL_STATE.glob('*.json')}


def main():
    subprocess.run([str(BIN), '--quit'], check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    wait_for(lambda: not (REAL_STATE / 'control.sock').exists(), 3)
    protected = state_hash()
    results = {}
    try:
        with tempfile.TemporaryDirectory(prefix='smart-switch-smoke-') as directory:
            temporary = Path(directory)
            for mode in ('fast', 'preview'):
                state = temporary / mode
                state.mkdir(mode=0o700)
                env = dict(SMART_SWITCH_STATE_DIR=str(state),
                           SMART_SWITCH_ENV_FILE=str(temporary / 'no-credentials'),
                           SMART_SWITCH_SMOKE_MODE=mode, BAML_TELEMETRY='off',
                           BAML_BRIDGE_DISABLE_DOWNLOAD='1')
                with (temporary / f'{mode}.err').open('w') as errors:
                    launch = ['/usr/bin/open', '-W', '-n', '-g', '--stderr', str(temporary / f'{mode}.app.err')]
                    for key in env:
                        launch.extend(['--env', f'{key}={env[key]}'])
                    launch.extend([str(APP), '--args', '--listen'])
                    process = subprocess.Popen(launch, stdout=subprocess.DEVNULL, stderr=errors)
                    try:
                        assert wait_for(lambda: (state / 'control.sock').exists(), 4), 'listener did not start'
                        # Isolated LaunchServices instances may be attributed differently
                        # by TCC. This test exercises the app's own control API, not keys.
                        activity = state / 'activity.json'
                        control(state, 'tap')
                        assert wait_for(lambda: activity.exists() and len(json.loads(activity.read_text()).get('switches', [])) == 1, 4), 'warmup did not settle'
                        original = front()
                        started = time.monotonic()
                        control(state, 'tap')
                        if mode == 'fast':
                            assert wait_for(lambda: front() != original, .9), 'confident first tap did not activate promptly'
                            results['fast_activation_ms'] = round((time.monotonic() - started) * 1000)
                            applied = front()
                            control(state, 'tap')
                            until = time.monotonic() + .8
                            while time.monotonic() < until:
                                assert front() == applied, 'correction tap activated an intermediate app'
                                time.sleep(.025)
                            assert wait_for(lambda: front() != applied, 1.4), 'corrected choice did not settle'
                        else:
                            until = time.monotonic() + .7
                            while time.monotonic() < until:
                                assert front() == original, 'uncertain choice activated before settling'
                                time.sleep(.025)
                            assert wait_for(lambda: front() != original, 1.5), 'uncertain choice did not settle'
                        activity = state / 'activity.json'
                        assert wait_for(lambda: activity.exists() and len(json.loads(activity.read_text()).get('switches', [])) == 2, 1)
                        event = json.loads(activity.read_text())['switches'][-1]
                        assert event['advances'] == (1 if mode == 'fast' else 0)
                        assert event['fast'] == (mode == 'fast')
                        if mode == 'fast':
                            assert 0 <= event['first_activation_ms'] < 900
                            assert event['settled_activation_ms'] > event['first_activation_ms'] + 700
                        else:
                            assert event['first_activation_ms'] >= 1400
                        assert event['learning'] is not None
                        results[f'{mode}_timing_recorded'] = True
                        results[f'{mode}_passed'] = True
                    finally:
                        if process.poll() is None:
                            control(state, 'quit')
                            process.wait(timeout=5)
            results['personal_history_untouched'] = state_hash() == protected
            assert results['personal_history_untouched']
    finally:
        restore = ['/usr/bin/open', '-n', str(DEFAULT_APP)]
        for key in ('SMART_SWITCH_ENV_FILE', 'BOUNDARY_PROJECT'):
            if key in os.environ:
                restore.extend(['--env', f'{key}={os.environ[key]}'])
        subprocess.run(restore + ['--args', '--listen'], check=True)
    print(json.dumps(results))


if __name__ == '__main__':
    main()
