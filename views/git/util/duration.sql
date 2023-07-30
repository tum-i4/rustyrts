-- this view calculates some absolute and relative e2e testing time statistics
CREATE MATERIALIZED VIEW duration as
(
SELECT r.path,
       r.id as repo_id,
       ROUND(CAST(avg(t.retest_all_duration) as numeric), 2)                         as retest_all_mean,
       ROUND(CAST(stddev(t.retest_all_duration) as numeric), 2)                      as retest_all_stddev,

       ROUND(CAST(avg(t.dynamic_duration) as numeric), 2)                            as dynamic_mean,
       ROUND(CAST(stddev(t.dynamic_duration) as numeric), 2)                         as dynamic_stddev,

       ROUND(CAST(avg(t.static_duration) as numeric), 2)                             as static_mean,
       ROUND(CAST(stddev(t.static_duration) as numeric), 2)                          as static_stddev,

       ROUND(CAST(avg(t.dynamic_duration / t.retest_all_duration) as numeric), 2)    as dynamic_mean_relative,
       ROUND(CAST(stddev(t.dynamic_duration / t.retest_all_duration) as numeric), 2) as dynamic_stddev_relative,

       ROUND(CAST(avg(t.static_duration / t.retest_all_duration) as numeric), 2)     as static_mean_relative,
       ROUND(CAST(stddev(t.static_duration / t.retest_all_duration) as numeric), 2)  as static_stddev_relative

FROM testreport_extended t
         join "Commit" C on t.repo_id = C.repo_id
         join "Repository" r on r.id = c.repo_id
GROUP BY r.id);