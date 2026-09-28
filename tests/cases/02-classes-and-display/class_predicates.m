% covers: 17 - islogical, ischar, isnumeric and isa test the class
% A logical is not numeric: isnumeric(true) and isa(true, 'numeric') are 0.
% Each result is a logical, so disp prints it four wide.
disp(islogical(true))
disp(ischar('a'))
disp(isnumeric('a'))
disp(isnumeric(true))
disp(isnumeric(2))
disp(isa(2, 'double'))
disp(isa(true, 'numeric'))
disp(isa(2, 'numeric'))
