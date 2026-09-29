% covers: 12 - (Design notes, bounded work) a plot that would take a figure past 16,777,216 points is refused before anything is copied, exit 1
% The budget is counted from the arguments, read in place, before the
% data is copied, so the refusal costs only the zeros the script made.
% No file is written.
x = zeros(1, 16777217);
plot(x);
