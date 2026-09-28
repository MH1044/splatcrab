% covers: 15 - a statement after a local function in a script is MATLAB's error that function definitions must come at the end
% The error names the misplaced statement's line, 6, and nothing runs.
x = 1;
function f()
end
y = 2;
