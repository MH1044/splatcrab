% covers: 11 - a function that returns without assigning its declared output is the output-not-assigned error
% The check is made at the call, so the error names the script's line 3.
z = bad(1)
function y = bad(x)
end
