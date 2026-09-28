% covers: 14 - NaN cannot be converted to a logical
% QA D5: `if NaN` is taken as true today, so the 7 prints and the script exits
% 0. MATLAB and Octave both refuse: "NaN's cannot be converted to logicals."
% The disp inside the branch is what makes the old behaviour visible: an empty
% body would print nothing either way.
if NaN, disp(7), end
disp(8)
