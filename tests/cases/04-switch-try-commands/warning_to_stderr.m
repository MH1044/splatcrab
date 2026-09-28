% covers: 13 - warning formats its message and writes it to stderr, and the script runs on to exit 0
% The disp shows that the script continues past the warning and that stdout
% holds its output alone.
warning('careful %d', 1)
disp(2)
