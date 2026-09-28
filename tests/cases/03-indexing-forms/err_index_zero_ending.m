% covers: 11 - end+1 growth and end deletion; x(0) is an error ending "or logical values."
% The new ending discharges the Known deviations row that scheduled it to this
% cycle. The count of 5 is printed before the error, so stdout holds it.
x = 1:5;
x(end+1) = 6;
x(end) = [];
disp(numel(x))
x(0)
