```bpmn
<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="order-review" name="주문 검토">
    <startEvent id="start" name="주문 접수"/>
    <userTask id="review" name="주문 검토"/>
    <endEvent id="end" name="완료"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="review"/>
    <sequenceFlow id="f2" sourceRef="review" targetRef="end"/>
  </process>
</definitions>
```

```bpmn
title: 주문 접수와 기록
participants:
  - customer: 고객
  - shop:
      name: 판매사
      nodes:
        - received:
            kind: startEvent
            name: 주문 접수
        - review:
            kind: userTask
            name: 주문 검토
        - order_doc:
            kind: dataObjectReference
            name: 주문서
        - note:
            kind: textAnnotation
            name: 30분 안에 검토
        - done:
            kind: endEvent
            name: 검토 완료
flows:
  - customer --> received
  - received --> review
  - review --> order_doc
  - note --> review
  - review --> done
```

```bpmn
<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <collaboration id="c1" name="주문 처리 협업">
    <participant id="customer" name="고객"/>
    <participant id="seller" name="판매사" processRef="seller-process"/>
    <messageFlow id="mf1" name="주문" sourceRef="customer" targetRef="received"/>
  </collaboration>
  <process id="seller-process">
    <laneSet>
      <lane id="sales" name="영업">
        <flowNodeRef>received</flowNodeRef>
        <flowNodeRef>review</flowNodeRef>
        <flowNodeRef>timeout</flowNodeRef>
        <flowNodeRef>notify</flowNodeRef>
        <flowNodeRef>cancel</flowNodeRef>
        <flowNodeRef>notified</flowNodeRef>
      </lane>
      <lane id="warehouse" name="창고">
        <flowNodeRef>check_stock</flowNodeRef>
        <flowNodeRef>prepare</flowNodeRef>
        <flowNodeRef>shipped</flowNodeRef>
      </lane>
    </laneSet>
    <startEvent id="received">
      <messageEventDefinition/>
    </startEvent>
    <userTask id="review" name="주문 검토"/>
    <boundaryEvent id="timeout" name="시한 초과" attachedToRef="review">
      <timerEventDefinition/>
    </boundaryEvent>
    <endEvent id="cancel" name="주문 취소"/>
    <exclusiveGateway id="check_stock" name="재고 있음?"/>
    <userTask id="prepare" name="출고 준비"/>
    <serviceTask id="notify" name="품절 안내 발송"/>
    <endEvent id="shipped" name="출고 완료"/>
    <endEvent id="notified" name="안내 완료"/>
    <sequenceFlow id="f1" sourceRef="received" targetRef="review"/>
    <sequenceFlow id="f2" sourceRef="review" targetRef="check_stock"/>
    <sequenceFlow id="f3" sourceRef="timeout" targetRef="cancel"/>
    <sequenceFlow id="f4" name="예" sourceRef="check_stock" targetRef="prepare"/>
    <sequenceFlow id="f5" name="아니오" sourceRef="check_stock" targetRef="notify"/>
    <sequenceFlow id="f6" sourceRef="prepare" targetRef="shipped"/>
    <sequenceFlow id="f7" sourceRef="notify" targetRef="notified"/>
  </process>
</definitions>
```
