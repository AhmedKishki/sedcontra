# Proposal: add an index for the order-history query

The order-history page takes 4 seconds to load at the 95th percentile. A trace of the page attributes 3.9 of those 4 seconds to one query, which selects a customer's orders by `customer_id` and sorts them by `created_at`. The query plan shows a full scan of the `orders` table, which holds 40 million rows. A full scan reads every row, so the query's time grows with the size of the table.

The `orders` table was created in 2019, when the shop moved its records out of a spreadsheet. The team that built it has since moved to the payments group.

A composite index on `(customer_id, created_at)` lets the database find one customer's rows directly and read them in sorted order. The largest customer has 9,000 orders, so with the index the query reads at most 9,000 rows where it now reads 40 million. On a copy of the production table, the indexed query for that customer ran in 30 milliseconds.

The index costs disk space and slows writes. It takes 1.1 GB on the copy, and the volume has 200 GB free. An insert into `orders` took 0.2 milliseconds longer with the index, and the table receives 30 inserts per second at peak, so the index adds 6 milliseconds of write time to each second of peak load.

We therefore propose to add the index. The trace puts the page's time in the query, the query's time comes from the scan, and the index removes the scan. Its costs are 1.1 GB of 200 GB free and 6 milliseconds per second of writes, which is small beside the 3.87 seconds it saves on each slow page load.
