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
}


@pytest.mark.parametrize(
    "program",
    sorted(TESTS_DIR.glob("*.q")),
    ids=lambda p: p.stem,
)
def test_program(program: Path):
    expected = TEST_CASES[program.name]

    result = subprocess.run(
        [str(Q_BIN), str(program)],
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

