-- this file contains some indexes that may speed up some of the analysis queries

create index if not exists "TestReport_name_index"
    on "TestReport" (name);

create index if not exists "TestReport_commit_str_index"
    on "TestReport" (commit_str);

create index if not exists "TestCase_id_suite_id_status_index"
    on "TestCase" (id, suite_id, status);

create index if not exists "TestSuite_id_report_id_name_index"
    on "TestSuite" (id, report_id, name);

create index if not exists "TestSuite_crashed_index"
    on "TestSuite" (crashed);

create index if not exists "TestCase_name_index"
    on "TestCase" (name);

create index if not exists "TestCase_status_index"
    on "TestCase" (status);

create index if not exists "TestSuite_name_index"
    on "TestSuite" (name);
