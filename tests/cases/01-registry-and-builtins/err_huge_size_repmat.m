% covers: 14 - repmat with a huge tile count is a clean error, not a panic
% NOTE: the message names the array asked for, 1x2 tiled 1e10 by 1e10, rather
% NOTE: than either factor on its own.
repmat([1 2], 1e10, 1e10)
