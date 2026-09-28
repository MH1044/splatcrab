% covers: 1 - a logical scalar displays with the logical header and width
% A comparison returns a logical, so the display gains the `logical` header and
% disp uses the four-wide logical column rather than the six-wide double one
% (QA D38).
x = 5 > 3
disp(3 > 1)
