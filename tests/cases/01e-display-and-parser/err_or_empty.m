% covers: 13 - and the same for an empty operand of ||
% The empty is written `[]` and never `1:0`, and it is `isempty` that is
% asserted, never a shape: which empty shape a value carries is cycle 02's
% business, and this case must not pin one. [] || 1 gives 1 today.
disp(isempty([]))
disp([] || 1)
