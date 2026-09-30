% covers: 2 - large values: all ones read as a signed 32-bit and 64-bit pattern are -1, the largest unsigned 32-bit value is exact, S4's int64 row is held as the nearest doubles, and 0x20000000000000 is exactly 2^53
fprintf('%d\n', 0xFFFFFFFFs32)
fprintf('%d\n', 0xFFFFFFFFu32)
fprintf('%d\n', 0xFFFFFFFFFFFFFFFFs64)
fprintf('%d %d\n', [0xFF000000001F123As64 0x1234FFFFFFFFFFFs64])
disp(0x20000000000000 == 2^53)
