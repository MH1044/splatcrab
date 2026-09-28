% covers: 4 - nargout is 1 when the caller uses one output and 0 at statement level, where the value still goes to ans and is displayed
disp(h()); h
function r = h()
r = nargout;
end
