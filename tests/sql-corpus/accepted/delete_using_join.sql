DELETE FROM public.accounts AS a
USING public.orders AS o
WHERE a.id = o.account_id;
