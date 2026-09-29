% covers: 7 - (Scope, datestr) a date past datestr's range is a clean error, never an overflow panic
% datestr([1e17 1 1 0 0 0]) exited 101 before cycle 13's review fix
datestr([1e17 1 1 0 0 0])
