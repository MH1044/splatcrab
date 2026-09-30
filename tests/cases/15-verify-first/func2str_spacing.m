% covers: 12 - func2str drops the space after an anonymous function's parameter list, as S11's examples show, and gives a named function's handle as its name alone
% The values are S11's examples and the spec's Acceptance item 12.
disp(func2str(@(x) x.^2+7))
fh = @(x,y)sqrt(x.^2+y.^2); disp(['Anonymous function: ' func2str(fh)])
disp(func2str(@cos))
