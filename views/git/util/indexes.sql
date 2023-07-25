create index "TestReport_name_index"
    on "TestReport" (name);

create index "TestReport_commit_str_index"
    on "TestReport" (commit_str);

create index "TestCase_id_suite_id_status_index"
    on "TestCase" (id, suite_id, status);

create index "TestSuite_id_report_id_name_index"
    on "TestSuite" (id, report_id, name);

create index "TestSuite_crashed_index"
    on "TestSuite" (crashed);

create index "TestCase_name_index"
    on "TestCase" (name);

create index "TestCase_status_index"
    on "TestCase" (status);

create index "TestSuite_name_index"
    on "TestSuite" (name);
