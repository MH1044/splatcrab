% covers: 7 - (Scope) pinv takes a tolerance, and one that is not a real scalar is a clean error with SplatCrab's own text, exit 1
% The tolerance 0.5 drops the singular value 0.1, so only the 1 is inverted.
disp(pinv(diag([1 0.1]), 0.5))
pinv(eye(2), [1 2])
