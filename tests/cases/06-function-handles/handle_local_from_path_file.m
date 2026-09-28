% covers: 13 - a handle to a function local to this script, called from a path file's function, still calls this script's function: it keeps the binding it was created with
% callh.m, beside this script, has a subfunction of the same name, twist,
% which is what the name means inside that file: callh's second output calls
% it by name and gets -1, while the handle it was given still reaches this
% script's twist and gets 5 + 100.
h = @twist;
[a, b] = callh(h, 5);
disp(a)
disp(b)
function r = twist(x)
r = x + 100;
end
