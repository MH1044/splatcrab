% covers: 11 - varargin is a 0x0 cell when a function receives no inputs after its declared ones, and 1x3 with three more, as S10's example function prints
% The function and its two calls are S10's, printing through fprintf.
definedAndVariableNumInputs(1, 2);
definedAndVariableNumInputs(1, 2, 3, 4, 5);
function definedAndVariableNumInputs(X, Y, varargin)
fprintf('Size of varargin cell array: %dx%d\n', size(varargin))
end
