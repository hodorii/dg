```mermaid
bpmn-beta

accTitle: Simple Linear Process

accDescr: A single user task between a start event and an end event.



start s1 "Start"

task:user t1 "Submit Request"

end e1 "Done"



s1 --> t1

t1 --> e1


```

```mermaid
bpmn-beta

accTitle: Purchase Request Approval

accDescr: A manager reviews a request and either approves or rejects it via an exclusive gateway.



start s1 "Request Raised"

task:user t1 "Review Request"

xor g1 "Approved?"

task:service t2 "Issue Purchase Order"

task:user t3 "Notify Rejection"

end e1 "Order Issued"

end e2 "Rejected"



s1 --> t1

t1 --> g1

g1 --> t2: "yes"

g1 --> t3: "no"

t2 --> e1

t3 ==> e2


```

```mermaid
bpmn-beta

accTitle: Purchase Order Collaboration

accDescr: Buyer and supplier pools exchange a purchase order via cross-pool message flow.



pool buyer "Buyer" {

  lane req "Requester" {

    start s1 "Need Identified"

    task:user t1 "Submit PO Request"

  }

  lane mgr "Manager" {

    task:user t2 "Review Request"

    xor g1 "Approved?"

    end e2 "Rejected"

  }

  s1 --> t1

  t1 --> t2

  t2 --> g1

  g1 --> e2: "no"

}



pool supplier "Supplier" {

  task:receive t3 "Receive PO"

  task t4 "Acknowledge PO"

  end e1 "PO Acknowledged"

  t3 --> t4

  t4 --> e1

}



g1 ~~> t3


```

```mermaid
bpmn-beta

accTitle: Order Fulfillment Process

accDescr: Customer places an order. The system validates it, checks inventory, and either fulfills or cancels.



start s1 "Order Placed"

task:user t1 "Validate Order"

xor g1 "Valid?"

task:service t2 "Check Inventory"

xor g2 "In Stock?"

task:service t3 "Pick and Pack"

task:user t4 "Notify Customer"

task:user t5 "Cancel Order"

end e1 "Order Shipped"

end e2 "Order Cancelled"



s1 --> t1

t1 --> g1

g1 --> t2: "valid"

g1 --> t5: "invalid"

t2 --> g2

g2 --> t3: "yes"

g2 --> t4: "no"

t3 --> e1

t4 ==> e2

t5 --> e2


```

```mermaid
bpmn-beta

accTitle: Parallel Task Processing

accDescr: Work is split in parallel across two service tasks and then joined before completion.



start s1 "Trigger"

and g1 "Split"

task:service t1 "Run Background Job"

task:service t2 "Send Notification"

and g2 "Join"

end e1 "Complete"



s1 --> g1

g1 --> t1

g1 --> t2

t1 --> g2

t2 --> g2

g2 --> e1


```

```mermaid
bpmn-beta

accTitle: Employee Onboarding

accDescr: HR initiates onboarding and runs a background check. If the check passes, IT and HR complete setup tasks in parallel. The hiring manager confirms the new hire is ready to start.



pool onboarding "Employee Onboarding" {



  lane hr "HR" {

    start s1 "Onboarding Initiated"

    task:service t1 "Send Offer Documentation"

    task:service t2 "Run Background Check"

    xor g1 "Background Check?"

    end e_fail "Onboarding Terminated"

    task:user t5 "Complete Orientation Paperwork"

  }



  lane it "IT" {

    task:service t3 "Provision System Accounts"

    task:service t4 "Provision Equipment"

  }



  lane hiring_manager "Hiring Manager" {

    and g2 "Parallel Setup Tasks"

    and g3 "Setup Complete"

    task:user t6 "Review Onboarding Checklist"

    task:user t7 "Confirm New Hire Ready"

    end e1 "New Hire Onboarded"

  }



  s1 --> t1

  t1 --> t2

  t2 --> g1

  g1 --> e_fail: "fail"

  g1 --> g2: "pass"

  g2 --> t3

  g2 --> t5

  t3 --> t4

  t4 --> g3

  t5 --> g3

  g3 --> t6

  t6 --> t7

  t7 --> e1

}


```

```mermaid
bpmn-beta LR
  pool "Order handling"
    lane "Sales"
      start message s1 "Order received"
      user task t1 "Approve order"
        boundary timer b1 "2 days"
      xor gw "Approved?"
    lane "Warehouse"
      service task t2 "Pick items"
      end e1 "Shipped"
  s1 --> t1 --> gw
  gw -- yes --> t2 --> e1
  b1 --> t2
```

```mermaid
bpmn-beta

accTitle: Cross-Pool Vendor Collaboration

accDescr: Company procurement sends a purchase order to a vendor. The vendor reviews and either accepts, sending an acknowledgement back, or rejects, sending a rejection notice.



pool company "Company" {



  lane proc "Procurement" {

    start s1 "Vendor Need Identified"

    task:user t1 "Create Purchase Order"

    task:send t2 "Send PO to Vendor"

    task:receive t7 "Receive Vendor Response"

    xor g2 "Response?"

    end e1 "PO Accepted"

    end e2 "PO Rejected: Resubmit"

  }



  s1 --> t1

  t1 --> t2

  t2 --> t7

  t7 --> g2

  g2 --> e1: "accepted"

  g2 --> e2: "rejected"

}



pool vendor "Vendor" {



  lane vendor_ops "Vendor Operations" {

    task:receive t3 "Receive Purchase Order"

    task:user t4 "Review PO Terms"

    xor g1 "Acceptable?"

    task:service t5 "Send Acceptance"

    task:service t6 "Send Rejection Notice"

    end e3 "PO Processed"

  }



  t3 --> t4

  t4 --> g1

  g1 --> t5: "yes"

  g1 --> t6: "no"

  t5 --> e3

  t6 --> e3

}



%% Message flows between pools -- top level

t2 ~~> t3

t5 ~~> t7

t6 ~~> t7


```

```mermaid
bpmn-beta

accTitle: Quote to Order

accDescr: Sales representative gathers requirements and generates a quote. Discounts above threshold require Sales Manager approval. Finance validates credit before the quote is sent. Accepted quotes are converted to confirmed sales orders.



pool q2o "Quote to Order" {



  lane rep "Sales Representative" {

    start s1 "Customer Inquiry Received"

    task:user t1 "Gather Customer Requirements and Confirm Product Specifications"

    task:user t2 "Generate Quote with Line Items, Pricing, and Applicable Discount"

    task:user t5 "Send Quote to Customer and Record Delivery in CRM"

    task:user t6 "Convert Accepted Quote to Confirmed Sales Order in ERP"

    end e1 "Sales Order Confirmed"

    end e3 "Quote Declined by Customer"

  }



  lane mgr "Sales Manager" {

    xor g1 "Discount Above Threshold?"

    task:user t3 "Review and Approve Discount Above Standard Threshold"

  }



  lane finance "Finance" {

    task:service t4 "Validate Customer Credit Limit Before Issuing Quote"

    xor g2 "Credit Approved?"

    xor g3 "Quote Accepted by Customer?"

    end e2 "Quote Withdrawn: Credit Declined"

  }



  s1 --> t1

  t1 --> t2

  t2 --> g1

  g1 --> t3: "yes -- above threshold"

  g1 --> t4: "no -- within limit"

  t3 --> t4

  t4 --> g2

  g2 --> t5: "approved"

  g2 --> e2: "declined"

  t5 --> g3

  g3 --> t6: "accepted"

  g3 --> e3: "declined"

  t6 --> e1

}


```

```mermaid
bpmn-beta

accTitle: Support Ticket Triage

accDescr: Customer submits a support ticket. Support team triages priority, handles high-priority tickets with parallel notification, and resolves all tickets before closing.



pool support "Customer Support" {



  lane customer "Customer" {

    start s1 "Support Ticket Submitted"

    task:user t1 "Describe Issue"

    task:receive t8 "Receive Resolution Confirmation"

    end e1 "Issue Resolved"

  }



  lane support_team "Support Team" {

    task:user t2 "Review Ticket"

    xor g1 "Priority?"

    and g2 "Parallel Notify"

    task:service t3 "Alert On-Call Engineer"

    task:service t4 "Create Escalation Ticket"

    and g3 "Notifications Complete"

    task:user t5 "Resolve High-Priority Issue"

    task:user t6 "Resolve Standard Issue"

    task:service t7 "Send Resolution to Customer"

  }



  s1 --> t1

  t1 --> t2

  t2 --> g1

  g1 --> g2: "high"

  g1 --> t6: "standard"

  g2 --> t3

  g2 --> t4

  t3 --> g3

  t4 --> g3

  g3 --> t5

  t5 --> t7

  t6 --> t7

  t7 --> t8

  t8 --> e1

}


```

