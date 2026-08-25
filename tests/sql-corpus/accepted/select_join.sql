SELECT *
FROM public.accounts AS a
CROSS JOIN public.orders AS o
WHERE a.id = o.account_id;
