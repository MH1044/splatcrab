% covers: 9 - a subfunction serves its own file: helper.m calls twice, but the script cannot
% twice(3) fails as a statement of this script, at the script's own level,
% so it names this script's line: 4, counting this covers line as line 1.
disp(helper(3)); twice(3)
