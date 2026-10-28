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

    # Unsigned integer remainder
    "u8_remainder.q": {
        "ret": 0,
        "stdout": ["4", "7", "1", "0", "0", "0", "0", "0", "1"],
        "stderr": [],
    },
    "u16_remainder.q": {
        "ret": 0,
        "stdout": ["80", "128", "1", "0", "0", "0", "0", "0", "1"],
        "stderr": [],
    },
    "u32_remainder.q": {
        "ret": 0,
        "stdout": ["10240", "65536", "1", "0", "0", "0", "0", "0", "1"],
        "stderr": [],
    },
    "u64_remainder.q": {
        "ret": 0,
        "stdout": ["3305635840", "4294967296", "1", "0", "0", "0", "0", "0", "1"],
        "stderr": [],
    },

    # Signed integer remainder
    "i8_remainder.q": {
        "ret": 0,
        "stdout": ["-2", "7", "-1", "1", "-3", "0", "0", "0", "0", "1"],
        "stderr": [],
    },
    "i16_remainder.q": {
        "ret": 0,
        "stdout": ["-49", "256", "-6", "4", "-7", "0", "0", "0", "0", "0"],
        "stderr": [],
    },
    "i32_remainder.q": {
        "ret": 0,
        "stdout": ["-37889", "65536", "-6", "2", "-7", "0", "0", "0", "0", "1"],
        "stderr": [],
    },
    "i64_remainder.q": {
        "ret": 0,
        "stdout": ["-3800301569", "4294967296", "-3", "4", "-7", "0", "0", "0", "0", "0"],
        "stderr": [],
    },

    # Unsigned integer division
    "u8_division.q": {
        "ret": 0,
        "stdout": ["28", "0", "127", "85", "200", "0", "1", "1", "0"],
        "stderr": [],
    },
    "u16_division.q": {
        "ret": 0,
        "stdout": ["390", "0", "32767", "21845", "50000", "0", "1", "1", "0"],
        "stderr": [],
    },
    "u32_division.q": {
        "ret": 0,
        "stdout": ["61035", "0", "2147483647", "1431655765", "4000000000", "0", "1", "1", "0"],
        "stderr": [],
    },
    "u64_division.q": {
        "ret": 0,
        "stdout": ["4190951585", "0", "9223372036854775807", "6148914691236517205", "18000000000000000000", "0", "1", "1", "0"],
        "stderr": [],
    },

    # Signed integer division
    "i8_division.q": {
        "ret": 0,
        "stdout": ["-14", "0", "33", "-2", "0", "0", "127", "-127", "1", "-42"],
        "stderr": [],
    },
    "i16_division.q": {
        "ret": 0,
        "stdout": ["-117", "0", "4285", "-36", "0", "0", "32767", "-32767", "1", "-4681"],
        "stderr": [],
    },
    "i32_division.q": {
        "ret": 0,
        "stdout": ["-30517", "0", "285714285", "-9362", "0", "0", "2147483647", "-2147483647", "1", "-306783378"],
        "stderr": [],
    },
    "i64_division.q": {
        "ret": 0,
        "stdout": ["-2095475792", "0", "1285714285714285714", "-613566756", "0", "0", "9223372036854775807", "-9223372036854775807", "1", "-1317624576693539401"],
        "stderr": [],
    },

    # Floating-point division
    "f32_division.q": {
        "ret": 0,
        "stdout": ["3", "-2", "-6", "-0.5", "5", "0", "1", "0.5", "7.5"],
        "stderr": [],
    },
    "f64_division.q": {
        "ret": 0,
        "stdout": ["5.25", "-14", "-0.375", "4", "0", "1", "8388608.5", "0.25", "21"],
        "stderr": [],
    },

    # Unsigned integer multiplication
    "u8_multiplication.q": {
        "ret": 0,
        "stdout": ["255", "255", "0", "255", "0", "15", "119"],
        "stderr": [],
    },
    "u16_multiplication.q": {
        "ret": 0,
        "stdout": ["65535", "65535", "0", "65535", "0", "255", "32896"],
        "stderr": [],
    },
    "u32_multiplication.q": {
        "ret": 0,
        "stdout": ["4294967295", "4294967295", "0", "4294967295", "0", "65535", "2147516416"],
        "stderr": [],
    },
    "u64_multiplication.q": {
        "ret": 0,
        "stdout": ["18446744073709551615", "18446744073709551615", "0", "18446744073709551615", "0", "4294967295", "281474976776192"],
        "stderr": [],
    },

    # Signed integer multiplication
    "i8_multiplication.q": {
        "ret": 0,
        "stdout": ["-84", "-84", "60", "-35", "0", "-128", "127"],
        "stderr": [],
    },
    "i16_multiplication.q": {
        "ret": 0,
        "stdout": ["-32640", "-32640", "128", "-255", "0", "-32768", "32767"],
        "stderr": [],
    },
    "i32_multiplication.q": {
        "ret": 0,
        "stdout": ["-2147395600", "-2147395600", "46340", "-46340", "0", "-2147483647", "2147483647"],
        "stderr": [],
    },
    "i64_multiplication.q": {
        "ret": 0,
        "stdout": ["-9223372030926249001", "-9223372030926249001", "3037000499", "-3037000499", "0", "-9223372036854775807", "9223372036854775807"],
        "stderr": [],
    },

    # Floating-point multiplication
    "f32_multiplication.q": {
        "ret": 0,
        "stdout": ["3.75", "3.75", "-1.875", "-3.125", "1.5625", "0.75", "1.25"],
        "stderr": [],
    },
    "f64_multiplication.q": {
        "ret": 0,
        "stdout": ["3.125", "3.125", "-1.875", "-3.75", "2.25", "0.625", "8388608.5"],
        "stderr": [],
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
        "stdout": ["-42.5", "-17.25", "42.5", "17.25"],
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

