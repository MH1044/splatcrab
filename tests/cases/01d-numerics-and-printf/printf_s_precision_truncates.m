% covers: 16 - printf applies a precision to %s, truncating before it pads
% C and MATLAB read %5.2s as "take two characters, then pad to five", which is
% [   ab]. The old code ignored the precision and printed the whole string.
fprintf('[%5.2s]\n', 'abcdef');
