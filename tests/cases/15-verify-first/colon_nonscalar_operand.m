% covers: 10 - a colon operand that is not a scalar is an error naming the operand, range start, range step or range end, an empty operand included and in a for range alike; 1:size(ones(3, 4)) is a range end that is not a scalar
% The texts are the spec's (S1 and Scope, a colon operand that is not a
% scalar); each is caught and printed on one line. The lines after the for
% loop over 1:[] take the same rule to an empty start and step and to a
% for range's start and step.
try, x = [1 3]:4; catch e, disp(e.message); end
try, x = [1 2 3]:2:10; catch e, disp(e.message); end
try, x = 1:[1 2]:5; catch e, disp(e.message); end
try, x = 1:[3 4]; catch e, disp(e.message); end
try, x = 1:[]; catch e, disp(e.message); end
try, x = 1:size(ones(3, 4)); catch e, disp(e.message); end
try
  for k = 1:[], disp(k), end
catch e
  disp(e.message);
end
try, x = []:3; catch e, disp(e.message); end
try, x = 1:[]:3; catch e, disp(e.message); end
try
  for k = [1 2]:3, disp(k), end
catch e
  disp(e.message);
end
try
  for k = 1:[1 2]:3, disp(k), end
catch e
  disp(e.message);
end
