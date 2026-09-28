% covers: 9 - a function whose parameter is varargin takes any number of arguments, and nargin counts every one
disp(cnt(1, 2, 3)); disp(cnt())
function r = cnt(varargin)
r = nargin;
end
