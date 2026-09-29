% covers: 9 - (Scope, conv) conv with a shape other than 'full', 'same' or 'valid' is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
c = conv([1 2], [1 2], 'middle');
