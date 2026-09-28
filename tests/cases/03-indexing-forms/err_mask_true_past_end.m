% covers: 19 - a mask with a true past the end is an out-of-bounds error on read, as a numeric index would be
% From the Scope's find(mask) bullet. The spec gives the rule, not the
% sentence, so only the clean exit is asserted: 1, not a panic, and no output.
x = [1 2];
x(logical([0 0 1]))
