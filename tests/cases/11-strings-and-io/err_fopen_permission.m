% covers: 8 - fopen with a permission that is not r, w, a, r+, w+ or a+, with at most one t or b, is a clean error, exit 1, and creates no file
% The text is SplatCrab's own.
fid = fopen('err_fopen_permission.txt', 'rw');
