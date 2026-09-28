% covers: 2 - a chained range parses left-associatively: 1:2:3:4 is (1:2:3):4
% `parse_range` handles at most two colons and does not loop, so both lines
% below are parse errors today ("expected ')' but found Colon") rather than
% ranges at all.
%
% The first line keeps every operand scalar, so the chained form has a value:
% (1:1:1) is the scalar 1, and 1:3 is [1 2 3].
%
% The second is the spec's own example. (1:2:3) is [1 3], so the chained form
% asks for a range whose start is a 1x2, and this interpreter refuses that
% with `range start must be a scalar.` -- exactly what `disp((1:2:3):4)`
% reports today. The bullet asks that the two spellings agree, and that is
% what is pinned here: the parenthesised form's behaviour, reached through the
% chain. (MATLAB's own colon takes the first element instead; letting the
% colon accept a non-scalar is not in this cycle's Scope.)
disp(1:1:1:3)
disp(1:2:3:4)
