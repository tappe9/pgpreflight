UPDATE public.accounts AS a
SET status = 'done'
FROM public.orders AS o
WHERE a.id = o.account_id;
