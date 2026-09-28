% covers: 10 - a script called from a function runs in that function's workspace, not the base one
% setup.m assigns a = 7 inside from_setup, which returns 2 * a; the base
% workspace never gets an a, so exist('a') is 0 there.
disp(from_setup())
disp(exist('a'))
function r = from_setup()
setup;
r = 2 * a;
end
