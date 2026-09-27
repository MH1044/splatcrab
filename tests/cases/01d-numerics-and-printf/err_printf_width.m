% covers: 7 - printf bounds its width, and refuses an absurd one instead of building the pad
% The width is judged before a single space is written, so this returns at
% once. The old code built a two-gigabyte pad and aborted in the allocator
% with exit 134; a case that only matched the message would have passed on a
% ten-second hang, so the exit code and the promptness are both the point.
% NOTE: the .err substring is deliberately short, for the reason given in
% err_printf_precision_f.m.
fprintf('%2147483647d', 1);
