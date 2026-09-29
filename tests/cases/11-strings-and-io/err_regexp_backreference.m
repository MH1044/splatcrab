% covers: 15 - a backreference cannot be matched in linear time and is refused with a clean error, exit 1
% NOTE: MATLAB matches backreferences; SplatCrab refuses them, the deviation
% NOTE: the spec records. The text is SplatCrab's own.
regexp('aa', '(a)\1')
