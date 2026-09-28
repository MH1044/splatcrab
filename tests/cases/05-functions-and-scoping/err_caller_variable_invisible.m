% covers: 6 - each function has its own workspace: g2's x is not the script's, and g3 cannot see the script's x
% The script's x survives g2's assignment to a variable of the same name,
% and g3's disp(x) is the unrecognized-name error with a stack line for g3.
% Lines count this covers line as line 1, so the failing disp(x) is line 12.
% The Error: prefix names the script's own statement, line 7, which called
% g3; the stack line names the line inside g3 that failed.
x = 1; g2(); disp(x); g3()
function g2()
x = 99;
end
function g3()
disp(x)
end
