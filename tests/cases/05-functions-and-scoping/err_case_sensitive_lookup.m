% covers: Scope, resolution - a file name must match the call exactly, on every platform:
% addone.m answers addone, never ADDONE. Windows and macOS file systems ignore case, and
% before cycle 05's review this call found the file there and failed on Linux
disp(addone(1))
ADDONE(1)
