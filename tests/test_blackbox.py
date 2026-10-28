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

    # comparison among diff types
    "comparison_diff_types.q": {
        "ret": 1,
        "stdout": [],
        "stderr": ["err: typechecker: Comparisons require operands of the same type, got Int32 and UInt64"],
    },

    # integer comparisons
    "u8_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false"],
        "stderr": [],
    },
    "u16_comparisons.q": {
        "ret": 0,
        "stdout": ["false", "false", "true", "true", "false", "true", "true", "false"],
        "stderr": [],
    },
    "u32_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false"],
        "stderr": [],
    },
    "u64_comparisons.q": {
        "ret": 0,
        "stdout": ["false", "false", "true", "true", "false", "true", "true", "false"],
        "stderr": [],
    },
    "i8_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false"],
        "stderr": [],
    },
    "i16_comparisons.q": {
        "ret": 0,
        "stdout": ["false", "false", "true", "true", "false", "true", "true", "false"],
        "stderr": [],
    },
    "i32_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false"],
        "stderr": [],
    },
    "i64_comparisons.q": {
        "ret": 0,
        "stdout": ["false", "false", "true", "true", "false", "true", "true", "false"],
        "stderr": [],
    },

    # float comparisons
    "f32_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false"],
        "stderr": [],
    },
    "f64_comparisons.q": {
        "ret": 0,
        "stdout": ["true", "true", "false", "false", "false", "true", "true", "false", "true", "true"],
        "stderr": [],
    },

    # i32 comparisons
    "less.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },
    "lessequal.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },
    "greater.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },
    "greaterequal.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },
    "equal.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },
    "notequal.q": {
        "ret": 0,
        "stdout": ["0"],
        "stderr": [],
    },

    # Unsigned -> Unsigned
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

    # Signed -> signed
    "i8_to_i16.q": {
        "ret": 0,
        "stdout": ["128", "-128"],
        "stderr": [],
    },
    "i8_to_i32.q": {
        "ret": 0,
        "stdout": ["128", "-128"],
        "stderr": [],
    },
    "i8_to_i64.q": {
        "ret": 0,
        "stdout": ["128", "-128"],
        "stderr": [],
    },
    "i16_to_i8.q": {
        "ret": 0,
        "stdout": ["0", "-1", "127"],
        "stderr": [],
    },
    "i16_to_i32.q": {
        "ret": 0,
        "stdout": ["32768", "-32768"],
        "stderr": [],
    },
    "i16_to_i64.q": {
        "ret": 0,
        "stdout": ["32768", "-32768"],
        "stderr": [],
    },
    "i32_to_i8.q": {
        "ret": 0,
        "stdout": ["0", "-1", "127"],
        "stderr": [],
    },
    "i32_to_i16.q": {
        "ret": 0,
        "stdout": ["0", "-1", "32767"],
        "stderr": [],
    },
    "i32_to_i64.q": {
        "ret": 0,
        "stdout": ["2147483648", "-2147483648"],
        "stderr": [],
    },
    "i64_to_i8.q": {
        "ret": 0,
        "stdout": ["0", "-1", "127"],
        "stderr": [],
    },
    "i64_to_i16.q": {
        "ret": 0,
        "stdout": ["0", "-1", "32767"],
        "stderr": [],
    },
    "i64_to_i32.q": {
        "ret": 0,
        "stdout": ["0", "-1", "2147483647"],
        "stderr": [],
    },

    # Float32 -> other types
    "f32_to_f64.q": {"ret": 0, "stdout": ["0.10000000149011612", "42.25"], "stderr": []},
    "f32_to_i8.q": {"ret": 0, "stdout": ["42", "-42", "127"], "stderr": []},
    "f32_to_i16.q": {"ret": 0, "stdout": ["42", "-42", "32767"], "stderr": []},
    "f32_to_i32.q": {"ret": 0, "stdout": ["42", "-42", "1073741824"], "stderr": []},
    "f32_to_i64.q": {"ret": 0, "stdout": ["42", "-42", "16777216"], "stderr": []},
    "f32_to_u8.q": {"ret": 0, "stdout": ["42", "0", "255"], "stderr": []},
    "f32_to_u16.q": {"ret": 0, "stdout": ["42", "0", "65535"], "stderr": []},
    "f32_to_u32.q": {"ret": 0, "stdout": ["42", "0", "2147483648"], "stderr": []},
    "f32_to_u64.q": {"ret": 0, "stdout": ["42", "0", "9223372036854775808"], "stderr": []},

    # Float64 -> other types
    "f64_to_f32.q": {"ret": 0, "stdout": ["16777216", "42.25"], "stderr": []},
    "f64_to_i8.q": {"ret": 0, "stdout": ["42", "-42", "127"], "stderr": []},
    "f64_to_i16.q": {"ret": 0, "stdout": ["42", "-42", "32767"], "stderr": []},
    "f64_to_i32.q": {"ret": 0, "stdout": ["42", "-42", "2147483647"], "stderr": []},
    "f64_to_i64.q": {"ret": 0, "stdout": ["42", "-42", "9007199254740992"], "stderr": []},
    "f64_to_u8.q": {"ret": 0, "stdout": ["42", "0", "255"], "stderr": []},
    "f64_to_u16.q": {"ret": 0, "stdout": ["42", "0", "65535"], "stderr": []},
    "f64_to_u32.q": {"ret": 0, "stdout": ["42", "0", "4294967295"], "stderr": []},
    "f64_to_u64.q": {"ret": 0, "stdout": ["42", "0", "9223372036854775808"], "stderr": []},

    # Int8 -> unsigned / floating-point
    "i8_to_u8.q": {"ret": 0, "stdout": ["255", "128"], "stderr": []},
    "i8_to_u16.q": {"ret": 0, "stdout": ["65535", "65408"], "stderr": []},
    "i8_to_u32.q": {"ret": 0, "stdout": ["4294967295", "4294967168"], "stderr": []},
    "i8_to_u64.q": {"ret": 0, "stdout": ["18446744073709551615", "18446744073709551488"], "stderr": []},
    "i8_to_f32.q": {"ret": 0, "stdout": ["-42", "127"], "stderr": []},
    "i8_to_f64.q": {"ret": 0, "stdout": ["-42", "127"], "stderr": []},

    # Int16 -> unsigned / floating-point
    "i16_to_u8.q": {"ret": 0, "stdout": ["255", "0", "128"], "stderr": []},
    "i16_to_u16.q": {"ret": 0, "stdout": ["65535", "32768"], "stderr": []},
    "i16_to_u32.q": {"ret": 0, "stdout": ["4294967295", "4294934528"], "stderr": []},
    "i16_to_u64.q": {"ret": 0, "stdout": ["18446744073709551615", "18446744073709518848"], "stderr": []},
    "i16_to_f32.q": {"ret": 0, "stdout": ["-42", "32767"], "stderr": []},
    "i16_to_f64.q": {"ret": 0, "stdout": ["-42", "32767"], "stderr": []},

    # Int32 -> unsigned / floating-point
    "i32_to_u8.q": {"ret": 0, "stdout": ["255", "0", "128"], "stderr": []},
    "i32_to_u16.q": {"ret": 0, "stdout": ["65535", "0", "32768"], "stderr": []},
    "i32_to_u32.q": {"ret": 0, "stdout": ["4294967295", "2147483648"], "stderr": []},
    "i32_to_u64.q": {"ret": 0, "stdout": ["18446744073709551615", "18446744071562067968"], "stderr": []},
    "i32_to_f32.q": {"ret": 0, "stdout": ["-42", "16777216"], "stderr": []},
    "i32_to_f64.q": {"ret": 0, "stdout": ["-42", "2147483647"], "stderr": []},

    # Int64 -> unsigned / floating-point
    "i64_to_u8.q": {"ret": 0, "stdout": ["255", "0", "128"], "stderr": []},
    "i64_to_u16.q": {"ret": 0, "stdout": ["65535", "0", "32768"], "stderr": []},
    "i64_to_u32.q": {"ret": 0, "stdout": ["4294967295", "0", "2147483648"], "stderr": []},
    "i64_to_u64.q": {"ret": 0, "stdout": ["18446744073709551615", "9223372036854775808"], "stderr": []},
    "i64_to_f32.q": {"ret": 0, "stdout": ["-42", "16777216"], "stderr": []},
    "i64_to_f64.q": {"ret": 0, "stdout": ["-42", "9007199254740992"], "stderr": []},

    # UInt8 -> signed / floating-point
    "u8_to_i8.q": {"ret": 0, "stdout": ["-1", "-128"], "stderr": []},
    "u8_to_i16.q": {"ret": 0, "stdout": ["255", "128"], "stderr": []},
    "u8_to_i32.q": {"ret": 0, "stdout": ["255", "128"], "stderr": []},
    "u8_to_i64.q": {"ret": 0, "stdout": ["255", "128"], "stderr": []},
    "u8_to_f32.q": {"ret": 0, "stdout": ["0", "255"], "stderr": []},
    "u8_to_f64.q": {"ret": 0, "stdout": ["0", "255"], "stderr": []},

    # UInt16 -> signed / floating-point
    "u16_to_i8.q": {"ret": 0, "stdout": ["0", "-1", "-128"], "stderr": []},
    "u16_to_i16.q": {"ret": 0, "stdout": ["-1", "-32768"], "stderr": []},
    "u16_to_i32.q": {"ret": 0, "stdout": ["65535", "32768"], "stderr": []},
    "u16_to_i64.q": {"ret": 0, "stdout": ["65535", "32768"], "stderr": []},
    "u16_to_f32.q": {"ret": 0, "stdout": ["0", "65535"], "stderr": []},
    "u16_to_f64.q": {"ret": 0, "stdout": ["0", "65535"], "stderr": []},

    # UInt32 -> signed / floating-point
    "u32_to_i8.q": {"ret": 0, "stdout": ["0", "-1", "-128"], "stderr": []},
    "u32_to_i16.q": {"ret": 0, "stdout": ["0", "-1", "-32768"], "stderr": []},
    "u32_to_i32.q": {"ret": 0, "stdout": ["-1", "-2147483648"], "stderr": []},
    "u32_to_i64.q": {"ret": 0, "stdout": ["4294967295", "2147483648"], "stderr": []},
    "u32_to_f32.q": {"ret": 0, "stdout": ["42", "16777216"], "stderr": []},
    "u32_to_f64.q": {"ret": 0, "stdout": ["42", "4294967295"], "stderr": []},

    # UInt64 -> signed / floating-point
    "u64_to_i8.q": {"ret": 0, "stdout": ["0", "-1", "-128"], "stderr": []},
    "u64_to_i16.q": {"ret": 0, "stdout": ["0", "-1", "-32768"], "stderr": []},
    "u64_to_i32.q": {"ret": 0, "stdout": ["0", "-1", "-2147483648"], "stderr": []},
    "u64_to_i64.q": {"ret": 0, "stdout": ["-1", "-9223372036854775808"], "stderr": []},
    "u64_to_f32.q": {"ret": 0, "stdout": ["42", "9223372000000000000"], "stderr": []},
    "u64_to_f64.q": {"ret": 0, "stdout": ["42", "9223372036854776000"], "stderr": []},

    "float_negation.q": {
        "ret": 0,
        "stdout": ["-42.5", "-17.25"],
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
    "do_while.q": {
        "ret": 0,
        "stdout": ["0", "1", "2", "3", "4"],
        "stderr": [],
    },
    "for_empty.q": {
        "ret": 0,
        "stdout": ["0", "1", "2", "3", "4"],
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

