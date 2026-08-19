WITH removed AS (DELETE FROM orders WHERE id = 1 RETURNING id) SELECT * FROM removed;
