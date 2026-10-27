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
        "stderr": [],
    },
    "fib.q": {
        "ret": 0,
        "stdout": ["55"],
        "stderr": [],
    },
    "fixup_cvtsi2ss_imm.q": {
        "ret": 0,
        "stdout": ["42"],
        "stderr": [],
    },
    "fixup_cvtsi2sd_imm.q": {
        "ret": 0,
        "stdout": ["42"],
        "stderr": [],
    },
    "fixup_movsx_imm.q": {
        "ret": 0,
        "stdout": ["0", "127", "32767", "2147483647"],
        "stderr": [],
    },
    "fixup_movzx_imm.q": {
        "ret": 0,
        "stdout": ["0", "127", "255", "65535", "4294967295"],
        "stderr": [],
    },
    "for.q": {
        "ret": 0,
        "stdout": ["0", "1", "2", "3", "4"],
        "stderr": [],
    },
    "loop.q": {
        "ret": 0,
        "stdout": ["0", "1", "2", "3", "4"],
        "stderr": [],
    },
    "no_main.q": {
        "ret": 1,
        "stderr": ["err: vm: No 'main' fn. (code: 1)"],
        "stdout": [],
    },
    "u8_to_u16.q": {
        "ret": 0,
        "stdout": ["256"],
        "stderr": [],
    },
    "u16_to_u8.q": {
        "ret": 0,
        "stdout": ["0", "255"],
        "stderr": [],
    },
    "u8_to_u32.q": {
        "ret": 0,
        "stdout": ["256"],
        "stderr": [],
    },
    "u32_to_u8.q": {
        "ret": 0,
        "stdout": ["0", "255"],
        "stderr": [],
    },
    "u8_to_u64.q": {
        "ret": 0,
        "stdout": ["256"],
        "stderr": [],
    },
    "u64_to_u8.q": {
        "ret": 0,
        "stdout": ["0", "255"],
        "stderr": [],
    },
    "u32_to_u64.q": {
        "ret": 0,
        "stdout": ["4294967296"],
        "stderr": [],
    },
    "u64_to_u32.q": {
        "ret": 0,
        "stdout": ["0", "4294967295"],
        "stderr": [],
    },
    "u16_to_u32.q": {
        "ret": 0,
        "stdout": ["65536"],
        "stderr": [],
    },
    "u32_to_u16.q": {
        "ret": 0,
        "stdout": ["0", "65535"],
        "stderr": [],
    },
    "u16_to_u64.q": {
        "ret": 0,
        "stdout": ["65536"],
        "stderr": [],
    },
    "u16_to_u64.q": {
        "ret": 0,
        "stdout": ["65536"],
        "stderr": [],
    },
    "u64_to_u16.q": {
        "ret": 0,
        "stdout": ["0", "65535"],
        "stderr": [],
    },
    "while.q": {
        "ret": 0,
        "stdout": ["0", "1", "2", "3", "4"],
        "stderr": [],
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

    expected_stderr = expected["stderr"]
    actual_stderr = result.stderr.splitlines()[-len(expected_stderr):]
    assert actual_stderr == expected_stderr

