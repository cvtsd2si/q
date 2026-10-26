import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
TESTS_DIR = ROOT / "tests" / "programs"
Q_BIN = ROOT / "target/release/q"

TEST_CASES = {
    "bool.q": {
        "ret": 0,
        "stdout": ["true", "false", "false", "true"],
    },
    "u8_to_u16.q": {
        "ret": 0,
        "stdout": ["256"],
    },
    "u16_to_u8.q": {
        "ret": 0,
        "stdout": ["0", "255"],
    },
}

@pytest.mark.parametrize(
    "jit",
    [False, True],
    ids=["vm", "jit"],
)
@pytest.mark.parametrize(
    "program",
    sorted(TESTS_DIR.glob("*.q")),
    ids=lambda p: p.stem,
)
def test_program(program: Path, jit: bool):
    cmd = [str(Q_BIN), str(program)]

    if jit:
        cmd.append("--jit")

    expected = TEST_CASES[program.name]

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=10,
    )

    assert result.returncode == expected["ret"], (
        f"Unexpected exit code.\n"
        f"stderr:\n{result.stderr}"
    )

    expected_stdout = expected["stdout"]
    actual_stdout = result.stdout.splitlines()[-len(expected_stdout):]

    assert actual_stdout == expected_stdout

