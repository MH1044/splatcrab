% covers: 18 - delete refuses a wildcard with a clean error, exit 1, rather than expand it; the case deletes its own file by name first, so it leaves nothing behind
% NOTE: MATLAB's delete expands a wildcard; SplatCrab refuses one, the
% NOTE: deviation the spec records. The text is SplatCrab's own.
fid = fopen('err_delete_wildcard.txt', 'w');
fclose(fid);
delete('err_delete_wildcard.txt');
delete('*.txt')
