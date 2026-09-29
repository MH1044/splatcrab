% covers: 6 - (Scope, Matrix.im) a complex value assigned into a char array is a clean error, exit 1, since no char is complex
% The text is SplatCrab's own, recorded in the spec's Design notes: the
% refusal names the class the value would have to become.
s = 'abc';
s(2) = 1i;
