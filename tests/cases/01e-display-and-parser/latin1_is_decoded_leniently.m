% covers: 9 - a non-UTF-8 file is decoded leniently, not refused
% The next comment line holds the Windows-1252 byte 0xE9 (e-acute), which
% is not valid UTF-8. A lossy decode keeps the script runnable.
% café au lait
disp(1)
