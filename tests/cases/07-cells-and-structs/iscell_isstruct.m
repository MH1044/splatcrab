% covers: 4 - (Scope, builtins) iscell and isstruct test for a cell and a struct, and isa names the classes cell and struct
% Each result is a logical, which disp prints four wide.
c = {1}; s.a = 1; disp(iscell(c)); disp(iscell(s)); disp(isstruct(s)); disp(isstruct(c)); disp(isa(c, 'cell')); disp(isa(s, 'struct'))
