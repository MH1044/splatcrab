% covers: 11 - calling a function with more arguments than it declares is the too-many-inputs error, raised at the call
% The call is a statement of this script and no frame for sq is entered, so
% the error names the script's line 4, counting this covers line as line 1.
sq(1, 2)
function y = sq(x)
    y = x^2;
end
